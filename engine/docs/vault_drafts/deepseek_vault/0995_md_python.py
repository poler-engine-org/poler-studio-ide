#!/usr/bin/env python3
"""
Setup для установки SSN как пакета
"""

from setuptools import setup, find_packages
import os

# Чтение README
with open('README.md', 'r', encoding='utf-8') as f:
long_description = f.read()

# Чтение требований
with open('requirements.txt', 'r', encoding='utf-8') as f:
requirements = f.read().splitlines()

setup(
name='sinusoidal-synaptic-network',
version='1.0.0',
author='SSN Team',
author_email='ssn@example.com',
description='Sinusoidal Synaptic Network - потоковая нейросеть на синапсах',
long_description=long_description,
long_description_content_type='text/markdown',
url='https://github.com/yourusername/ssn',

packages=find_packages(),
include_package_data=True,

classifiers=[
'Development Status :: 4 - Beta',
'Intended Audience :: Science/Research',
'Topic :: Scientific/Engineering :: Artificial Intelligence',
'License :: OSI Approved :: MIT License',
'Programming Language :: Python :: 3',
'Programming Language :: Python :: 3.8',
'Programming Language :: Python :: 3.9',
'Programming Language :: Python :: 3.10',
],

python_requires='>=3.8',
install_requires=requirements,

entry_points={
'console_scripts': [
'ssn-init=initialize_ssn:main',
'ssn-loader=ssn_loader:main',
'ssn-chat=interactive_conditional_samples:main',
'ssn-generate=generate_unconditional_samples:main',
],
},

keywords='ai neural-network synapse streaming semantic',
project_urls={
'Bug Reports': 'https://github.com/yourusername/ssn/issues',
'Source': 'https://github.com/yourusername/ssn',
},
)
